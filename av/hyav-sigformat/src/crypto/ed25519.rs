//! Pure Rust RFC 8032 Ed25519 signature verification and generation.
//! Zero third-party dependencies.

use super::sha512::sha512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ed25519Error {
    InvalidSignature,
    InvalidPublicKey,
    InvalidScalar,
}

// -----------------------------------------------------------------------------
// Field element mod 2^255 - 19 (radix 2^51, 5 limbs)
// -----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fe(pub [i64; 5]);

const MASK51: i64 = (1i64 << 51) - 1;

impl Fe {
    pub const ZERO: Fe = Fe([0; 5]);
    pub const ONE: Fe = Fe([1, 0, 0, 0, 0]);

    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        let mut limbs = [0i64; 5];
        let mut u = [0u8; 32];
        u.copy_from_slice(bytes);
        u[31] &= 0x7f; // clear highest bit for field element

        let mut bits = [0u8; 256];
        for i in 0..256 {
            bits[i] = (u[i / 8] >> (i % 8)) & 1;
        }

        for (limb_idx, limb) in limbs.iter_mut().enumerate() {
            let mut val = 0i64;
            for bit_idx in 0..51 {
                let bit = bits[limb_idx * 51 + bit_idx] as i64;
                val |= bit << bit_idx;
            }
            *limb = val;
        }

        let mut fe = Fe(limbs);
        fe.carry();
        fe
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        let mut t = *self;
        t.carry();
        t.carry();

        // Strict canonical reduction: if t >= p, t -= p
        let mut q = (19 + t.0[0]) >> 51;
        q = (q + t.0[1]) >> 51;
        q = (q + t.0[2]) >> 51;
        q = (q + t.0[3]) >> 51;
        q = (q + t.0[4]) >> 51;

        t.0[0] += 19 * q;
        t.carry();

        let mut out = [0u8; 32];
        let mut bits = [0u8; 256];
        for (limb_idx, limb) in t.0.iter().enumerate() {
            for bit_idx in 0..51 {
                bits[limb_idx * 51 + bit_idx] = ((limb >> bit_idx) & 1) as u8;
            }
        }
        for (i, b) in bits.iter().enumerate() {
            out[i / 8] |= b << (i % 8);
        }
        out
    }

    pub fn carry(&mut self) {
        let mut c = 0i64;
        for i in 0..5 {
            let val = self.0[i] + c;
            self.0[i] = val & MASK51;
            c = val >> 51;
        }
        self.0[0] += c * 19;
        let c0 = self.0[0] >> 51;
        self.0[0] &= MASK51;
        self.0[1] += c0;
    }

    pub fn add(&self, rhs: &Fe) -> Fe {
        let mut res = Fe([
            self.0[0] + rhs.0[0],
            self.0[1] + rhs.0[1],
            self.0[2] + rhs.0[2],
            self.0[3] + rhs.0[3],
            self.0[4] + rhs.0[4],
        ]);
        res.carry();
        res
    }

    pub fn sub(&self, rhs: &Fe) -> Fe {
        // Bias to keep positive before reduction: 2 * p in limbs
        const BIAS0: i64 = 0xffffffffffda0;
        const BIAS14: i64 = 0xffffffffffffe;
        let mut res = Fe([
            (self.0[0] + BIAS0) - rhs.0[0],
            (self.0[1] + BIAS14) - rhs.0[1],
            (self.0[2] + BIAS14) - rhs.0[2],
            (self.0[3] + BIAS14) - rhs.0[3],
            (self.0[4] + BIAS14) - rhs.0[4],
        ]);
        res.carry();
        res
    }

    pub fn mul(&self, rhs: &Fe) -> Fe {
        let a = self.0;
        let b = rhs.0;

        let m = |x: i64, y: i64| (x as i128) * (y as i128);

        // Standard radix 2^51 multiplication folded mod 2^255 - 19
        let r0 = m(a[0], b[0]) + 19 * (m(a[1], b[4]) + m(a[2], b[3]) + m(a[3], b[2]) + m(a[4], b[1]));
        let r1 = m(a[0], b[1]) + m(a[1], b[0]) + 19 * (m(a[2], b[4]) + m(a[3], b[3]) + m(a[4], b[2]));
        let r2 = m(a[0], b[2]) + m(a[1], b[1]) + m(a[2], b[0]) + 19 * (m(a[3], b[4]) + m(a[4], b[3]));
        let r3 = m(a[0], b[3]) + m(a[1], b[2]) + m(a[2], b[1]) + m(a[3], b[0]) + 19 * m(a[4], b[4]);
        let r4 = m(a[0], b[4]) + m(a[1], b[3]) + m(a[2], b[2]) + m(a[3], b[1]) + m(a[4], b[0]);

        let mask = MASK51 as i128;
        let c0 = r0 >> 51;
        let w0 = (r0 & mask) as i64;
        let r1_c = r1 + c0;

        let c1 = r1_c >> 51;
        let w1 = (r1_c & mask) as i64;
        let r2_c = r2 + c1;

        let c2 = r2_c >> 51;
        let w2 = (r2_c & mask) as i64;
        let r3_c = r3 + c2;

        let c3 = r3_c >> 51;
        let w3 = (r3_c & mask) as i64;
        let r4_c = r4 + c3;

        let c4 = r4_c >> 51;
        let w4 = (r4_c & mask) as i64;

        let mut res = Fe([w0 + (c4 as i64) * 19, w1, w2, w3, w4]);
        res.carry();
        res
    }

    pub fn sq(&self) -> Fe {
        self.mul(self)
    }

    pub fn inv(&self) -> Fe {
        // Fermat's Little Theorem: a^(p-2) mod p
        // p - 2 = 2^255 - 21
        self.pow_p_minus_2()
    }

    fn pow_p_minus_2(&self) -> Fe {
        let mut t0 = self.sq();
        let mut t1 = t0.sq();
        t1 = t1.sq();
        let mut t2 = self.mul(&t1);
        t0 = t0.mul(&t2);
        let mut t3 = t0.sq();
        t1 = t2.mul(&t3);
        let mut t4 = t1.sq();
        for _ in 1..5 {
            t4 = t4.sq();
        }
        t2 = t4.mul(&t1);
        t4 = t2.sq();
        for _ in 1..10 {
            t4 = t4.sq();
        }
        t3 = t4.mul(&t2);
        t4 = t3.sq();
        for _ in 1..20 {
            t4 = t4.sq();
        }
        t2 = t4.mul(&t3);
        t4 = t2.sq();
        for _ in 1..10 {
            t4 = t4.sq();
        }
        t1 = t4.mul(&t1);
        t4 = t1.sq();
        for _ in 1..50 {
            t4 = t4.sq();
        }
        t2 = t4.mul(&t2);
        t4 = t2.sq();
        for _ in 1..100 {
            t4 = t4.sq();
        }
        t2 = t4.mul(&t2);
        t4 = t2.sq();
        for _ in 1..50 {
            t4 = t4.sq();
        }
        t1 = t4.mul(&t1);
        t1 = t1.sq();
        t1 = t1.sq();
        t1.mul(self)
    }

    pub fn is_negative(&self) -> bool {
        let bytes = self.to_bytes();
        (bytes[0] & 1) != 0
    }
}

// d = -121665 / 121666 mod p
fn fe_d() -> Fe {
    let num = Fe::from_bytes(&{
        let mut b = [0u8; 32];
        b[0] = 0x61;
        b[1] = 0xdb;
        b[2] = 0x01; // 121665
        b
    });
    let den = Fe::from_bytes(&{
        let mut b = [0u8; 32];
        b[0] = 0x62;
        b[1] = 0xdb;
        b[2] = 0x01; // 121666
        b
    });
    let d_pos = num.mul(&den.inv());
    Fe::ZERO.sub(&d_pos)
}

// 2 * d
fn fe_2d() -> Fe {
    let d = fe_d();
    d.add(&d)
}

// -----------------------------------------------------------------------------
// Edwards Point in Extended Homogeneous Coordinates (X:Y:Z:T)
// x = X/Z, y = Y/Z, x*y = T/Z
// -----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    pub x: Fe,
    pub y: Fe,
    pub z: Fe,
    pub t: Fe,
}

impl Point {
    pub const IDENTITY: Point = Point {
        x: Fe::ZERO,
        y: Fe::ONE,
        z: Fe::ONE,
        t: Fe::ZERO,
    };

    pub fn basepoint() -> Self {
        // Basepoint y = 4/5 mod p
        let four = Fe::from_bytes(&{
            let mut b = [0u8; 32];
            b[0] = 4;
            b
        });
        let five = Fe::from_bytes(&{
            let mut b = [0u8; 32];
            b[0] = 5;
            b
        });
        let y = four.mul(&five.inv());
        // Recover x from y^2 - x^2 = 1 + d x^2 y^2
        // x^2 = (y^2 - 1) / (d y^2 + 1)
        let u = y.sq().sub(&Fe::ONE);
        let v = fe_d().mul(&y.sq()).add(&Fe::ONE);
        let mut x = recover_x(&u, &v).expect("Basepoint recovery");
        if x.is_negative() {
            x = Fe::ZERO.sub(&x);
        }
        let t = x.mul(&y);
        Point { x, y, z: Fe::ONE, t }
    }

    pub fn add(&self, other: &Point) -> Point {
        // Unified addition formulas (Hisil et al., 2008)
        let a = self.y.sub(&self.x).mul(&other.y.sub(&other.x));
        let b = self.y.add(&self.x).mul(&other.y.add(&other.x));
        let c = fe_2d().mul(&self.t).mul(&other.t);
        let d = self.z.mul(&other.z);
        let d2 = d.add(&d);
        let e = b.sub(&a);
        let f = d2.sub(&c);
        let g = d2.add(&c);
        let h = b.add(&a);
        Point {
            x: e.mul(&f),
            y: g.mul(&h),
            z: f.mul(&g),
            t: e.mul(&h),
        }
    }

    pub fn double(&self) -> Point {
        let a = self.x.sq();
        let b = self.y.sq();
        let c = self.z.sq();
        let c2 = c.add(&c);
        let d = Fe::ZERO.sub(&a);
        let e = self.x.add(&self.y).sq().sub(&a).sub(&b);
        let g = d.add(&b);
        let f = g.sub(&c2);
        let h = d.sub(&b);
        Point {
            x: e.mul(&f),
            y: g.mul(&h),
            z: f.mul(&g),
            t: e.mul(&h),
        }
    }

    pub fn scalar_mul(&self, scalar: &[u8; 32]) -> Point {
        let mut res = Point::IDENTITY;
        for i in (0..256).rev() {
            res = res.double();
            let bit = (scalar[i / 8] >> (i % 8)) & 1;
            if bit == 1 {
                res = res.add(self);
            }
        }
        res
    }

    pub fn compress(&self) -> [u8; 32] {
        let zinv = self.z.inv();
        let x = self.x.mul(&zinv);
        let y = self.y.mul(&zinv);
        let mut bytes = y.to_bytes();
        if x.is_negative() {
            bytes[31] |= 0x80;
        }
        bytes
    }

    pub fn decompress(bytes: &[u8; 32]) -> Result<Point, Ed25519Error> {
        let sign_bit = (bytes[31] >> 7) & 1;
        let mut ybytes = *bytes;
        ybytes[31] &= 0x7f;
        let y = Fe::from_bytes(&ybytes);

        // x^2 = (y^2 - 1) / (d y^2 + 1)
        let u = y.sq().sub(&Fe::ONE);
        let v = fe_d().mul(&y.sq()).add(&Fe::ONE);
        let x = match recover_x(&u, &v) {
            Some(x) => x,
            None => return Err(Ed25519Error::InvalidPublicKey),
        };

        let x = if (x.is_negative() as u8) != sign_bit {
            Fe::ZERO.sub(&x)
        } else {
            x
        };

        let t = x.mul(&y);
        Ok(Point { x, y, z: Fe::ONE, t })
    }
}

// Computes sqrt(u / v) mod p
fn recover_x(u: &Fe, v: &Fe) -> Option<Fe> {
    // candidate = u * v^3 * (u * v^7)^((p-5)/8)
    let v2 = v.sq();
    let v3 = v2.mul(v);
    let v7 = v3.mul(&v2).mul(&v2);
    let uv7 = u.mul(&v7);
    let pow = pow_p_minus_5_over_8(&uv7);
    let mut x = u.mul(&v3).mul(&pow);

    let check = v.mul(&x.sq());
    if check == *u {
        return Some(x);
    }

    // Try multiplying by sqrt(-1)
    let sqrt_minus_one = Fe::from_bytes(&[
        0xb0, 0xa0, 0x0e, 0x4a, 0x27, 0x1b, 0xee, 0xc4,
        0x78, 0xe4, 0x2f, 0xad, 0x06, 0x18, 0x43, 0x2f,
        0xa7, 0xd7, 0xfb, 0x3d, 0x99, 0x00, 0x4d, 0x2b,
        0x0b, 0xdf, 0x11, 0x43, 0x81, 0xe2, 0x48, 0x2b,
    ]);
    x = x.mul(&sqrt_minus_one);
    let check2 = v.mul(&x.sq());
    if check2 == *u {
        Some(x)
    } else {
        None
    }
}

fn pow_p_minus_5_over_8(a: &Fe) -> Fe {
    // (p - 5) / 8 = 2^252 - 3
    let mut t0 = a.sq();
    let mut t1 = t0.sq();
    t1 = t1.sq();
    let mut t2 = a.mul(&t1);
    t0 = t0.mul(&t2);
    let mut t3 = t0.sq();
    t1 = t2.mul(&t3);
    let mut t4 = t1.sq();
    for _ in 1..5 {
        t4 = t4.sq();
    }
    t2 = t4.mul(&t1);
    t4 = t2.sq();
    for _ in 1..10 {
        t4 = t4.sq();
    }
    t3 = t4.mul(&t2);
    t4 = t3.sq();
    for _ in 1..20 {
        t4 = t4.sq();
    }
    t2 = t4.mul(&t3);
    t4 = t2.sq();
    for _ in 1..10 {
        t4 = t4.sq();
    }
    t1 = t4.mul(&t1);
    t4 = t1.sq();
    for _ in 1..50 {
        t4 = t4.sq();
    }
    t2 = t4.mul(&t2);
    t4 = t2.sq();
    for _ in 1..100 {
        t4 = t4.sq();
    }
    t2 = t4.mul(&t2);
    t4 = t2.sq();
    for _ in 1..50 {
        t4 = t4.sq();
    }
    t1 = t4.mul(&t1);
    t1 = t1.sq();
    t1 = t1.sq();
    t1.mul(a)
}

// -----------------------------------------------------------------------------
// Scalar arithmetic mod L (order of basepoint)
// L = 2^252 + 27742317777372353535851937790883648493
// -----------------------------------------------------------------------------

const L: [u64; 4] = [
    0x5812631a5cf5d3ed,
    0x14def9dea2f79cd6,
    0x0000000000000000,
    0x1000000000000000,
];

fn scalar_reduce_wide(wide: &[u8; 64]) -> [u8; 32] {
    // Modulo reduction of 512-bit integer by L using standard long division
    let mut num = [0u64; 8];
    for i in 0..8 {
        let chunk = &wide[i * 8..(i + 1) * 8];
        num[i] = u64::from_le_bytes(chunk.try_into().unwrap());
    }

    // Division loop: bit-by-bit
    let mut rem = [0u64; 4];
    for bit in (0..512).rev() {
        // Shift rem left by 1
        let mut carry = ((num[bit / 64] >> (bit % 64)) & 1) != 0;
        for j in 0..4 {
            let next_carry = (rem[j] >> 63) != 0;
            rem[j] = (rem[j] << 1) | (if carry { 1 } else { 0 });
            carry = next_carry;
        }

        // If rem >= L or overflowed, rem -= L
        if carry || u256_gte(&rem, &L) {
            u256_sub_assign(&mut rem, &L);
        }
    }

    let mut out = [0u8; 32];
    for i in 0..4 {
        out[i * 8..(i + 1) * 8].copy_from_slice(&rem[i].to_le_bytes());
    }
    out
}

fn u256_gte(a: &[u64; 4], b: &[u64; 4]) -> bool {
    for i in (0..4).rev() {
        if a[i] > b[i] {
            return true;
        }
        if a[i] < b[i] {
            return false;
        }
    }
    true
}

fn u256_sub_assign(a: &mut [u64; 4], b: &[u64; 4]) {
    let mut borrow = 0u64;
    for i in 0..4 {
        let (diff1, b1) = a[i].overflowing_sub(b[i]);
        let (diff2, b2) = diff1.overflowing_sub(borrow);
        a[i] = diff2;
        borrow = (if b1 { 1 } else { 0 }) + (if b2 { 1 } else { 0 });
    }
}

fn scalar_mul_add(s: &[u8; 32], a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    // Compute (s + a * b) mod L
    let mut sa = [0u64; 4];
    let mut sb = [0u64; 4];
    let mut ss = [0u64; 4];
    for i in 0..4 {
        sa[i] = u64::from_le_bytes(a[i * 8..(i + 1) * 8].try_into().unwrap());
        sb[i] = u64::from_le_bytes(b[i * 8..(i + 1) * 8].try_into().unwrap());
        ss[i] = u64::from_le_bytes(s[i * 8..(i + 1) * 8].try_into().unwrap());
    }

    // 256x256 -> 512 bit multiply
    let mut prod = [0u64; 8];
    for i in 0..4 {
        let mut carry = 0u128;
        for j in 0..4 {
            let cur = (prod[i + j] as u128) + ((sa[i] as u128) * (sb[j] as u128)) + carry;
            prod[i + j] = cur as u64;
            carry = cur >> 64;
        }
        prod[i + 4] = carry as u64;
    }

    // Add ss to prod
    let mut carry = 0u64;
    for i in 0..4 {
        let (sum1, c1) = prod[i].overflowing_add(ss[i]);
        let (sum2, c2) = sum1.overflowing_add(carry);
        prod[i] = sum2;
        carry = (if c1 { 1 } else { 0 }) + (if c2 { 1 } else { 0 });
    }
    for i in 4..8 {
        let (sum, c) = prod[i].overflowing_add(carry);
        prod[i] = sum;
        carry = if c { 1 } else { 0 };
    }

    let mut wide = [0u8; 64];
    for i in 0..8 {
        wide[i * 8..(i + 1) * 8].copy_from_slice(&prod[i].to_le_bytes());
    }
    scalar_reduce_wide(&wide)
}

// -----------------------------------------------------------------------------
// Public Ed25519 API
// -----------------------------------------------------------------------------

pub struct Keypair {
    pub secret: [u8; 32],
    pub public: [u8; 32],
}

impl Keypair {
    pub fn from_seed(seed: &[u8; 32]) -> Self {
        let h = sha512(seed);
        let mut s = [0u8; 32];
        s.copy_from_slice(&h[0..32]);
        s[0] &= 248;
        s[31] &= 127;
        s[31] |= 64;

        let b = Point::basepoint();
        let pub_point = b.scalar_mul(&s);
        let public = pub_point.compress();

        Keypair {
            secret: *seed,
            public,
        }
    }
}

pub fn sign(secret_seed: &[u8; 32], message: &[u8]) -> [u8; 64] {
    let h = sha512(secret_seed);
    let mut s = [0u8; 32];
    s.copy_from_slice(&h[0..32]);
    s[0] &= 248;
    s[31] &= 127;
    s[31] |= 64;

    let b = Point::basepoint();
    let pub_point = b.scalar_mul(&s);
    let a_bytes = pub_point.compress();

    // r = SHA-512(h[32..64] || message) mod L
    let mut r_input = Vec::with_capacity(32 + message.len());
    r_input.extend_from_slice(&h[32..64]);
    r_input.extend_from_slice(message);
    let r_wide = sha512(&r_input);
    let r = scalar_reduce_wide(&r_wide);

    // R = r * B
    let r_point = b.scalar_mul(&r);
    let r_bytes = r_point.compress();

    // k = SHA-512(R || A || message) mod L
    let mut k_input = Vec::with_capacity(64 + message.len());
    k_input.extend_from_slice(&r_bytes);
    k_input.extend_from_slice(&a_bytes);
    k_input.extend_from_slice(message);
    let k_wide = sha512(&k_input);
    let k = scalar_reduce_wide(&k_wide);

    // S = (r + k * s) mod L
    let s_scalar = scalar_mul_add(&r, &k, &s);

    let mut sig = [0u8; 64];
    sig[0..32].copy_from_slice(&r_bytes);
    sig[32..64].copy_from_slice(&s_scalar);
    sig
}

pub fn verify(public_key: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> Result<(), Ed25519Error> {
    let mut r_bytes = [0u8; 32];
    let mut s_bytes = [0u8; 32];
    r_bytes.copy_from_slice(&signature[0..32]);
    s_bytes.copy_from_slice(&signature[32..64]);

    // Check S < L
    let mut s_u64 = [0u64; 4];
    for i in 0..4 {
        s_u64[i] = u64::from_le_bytes(s_bytes[i * 8..(i + 1) * 8].try_into().unwrap());
    }
    if u256_gte(&s_u64, &L) {
        return Err(Ed25519Error::InvalidScalar);
    }

    let a_point = Point::decompress(public_key)?;
    let r_point = Point::decompress(&r_bytes)?;

    // k = SHA-512(R || A || message) mod L
    let mut k_input = Vec::with_capacity(64 + message.len());
    k_input.extend_from_slice(&r_bytes);
    k_input.extend_from_slice(public_key);
    k_input.extend_from_slice(message);
    let k_wide = sha512(&k_input);
    let k = scalar_reduce_wide(&k_wide);

    // Check: S * B == R + k * A
    let sb = Point::basepoint().scalar_mul(&s_bytes);
    let ka = a_point.scalar_mul(&k);
    let r_plus_ka = r_point.add(&ka);

    if sb.compress() == r_plus_ka.compress() {
        Ok(())
    } else {
        Err(Ed25519Error::InvalidSignature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rfc8032_vector_1() {
        let seed = [
            0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60,
            0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c, 0xc4,
            0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19,
            0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae, 0x7f, 0x60,
        ];
        let kp = Keypair::from_seed(&seed);
        let expected_pub = [
            0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7,
            0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07, 0x3a,
            0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25,
            0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07, 0x51, 0x1a,
        ];
        assert_eq!(kp.public, expected_pub);

        let msg = b"";
        let sig = sign(&seed, msg);
        let expected_sig = [
            0xe5, 0x56, 0x43, 0x83, 0xc3, 0xe7, 0x03, 0xc8,
            0x82, 0x67, 0x5f, 0xf4, 0xf2, 0xd7, 0x54, 0xf8,
            0xce, 0x8e, 0x3b, 0x92, 0x79, 0xb2, 0xa1, 0xa3,
            0x9d, 0x1a, 0xa6, 0x2c, 0x14, 0x48, 0x16, 0xcd,
            0x4e, 0x14, 0xba, 0x52, 0xf7, 0x21, 0x59, 0x3f,
            0x00, 0x95, 0x30, 0x14, 0x15, 0x07, 0x16, 0x3d,
            0x01, 0x16, 0x69, 0x86, 0x87, 0x4e, 0x55, 0xe5,
            0xec, 0x29, 0x3b, 0x7e, 0x30, 0xf5, 0x07, 0x08,
        ];
        assert_eq!(sig, expected_sig);

        assert!(verify(&kp.public, msg, &sig).is_ok());
        // Corrupted msg must fail
        assert!(verify(&kp.public, b"tampered", &sig).is_err());
    }
}
