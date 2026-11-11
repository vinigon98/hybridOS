import subprocess
import os
import sys
import shutil
import tempfile

def get_vcvars_env(vcvars_path):
    cmd = f'"{vcvars_path}" x64 >nul 2>&1 && set'
    proc = subprocess.run(cmd, shell=True, capture_output=True, text=True)
    env = {}
    for line in proc.stdout.splitlines():
        if '=' in line:
            k, v = line.split('=', 1)
            env[k] = v
    return env

def main():
    root_dir = r"C:\Users\Vinicius\Documents\HybridOS-Kernel"
    vcvars = r"C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\VC\Auxiliary\Build\vcvarsall.bat"
    
    print("Initializing MSVC 64-bit environment...")
    msvc_env = get_vcvars_env(vcvars)
    if "PATH" not in msvc_env:
        print("Failed to initialize MSVC environment.")
        return 1
        
    cl_path = shutil.which("cl.exe", path=msvc_env["PATH"])
    print(f"Located cl.exe at: {cl_path}")
    if not cl_path:
        print("Error: cl.exe not found in MSVC PATH.")
        return 1

    temp_build_dir = os.path.join(tempfile.gettempdir(), "hybridos_build")
    os.makedirs(temp_build_dir, exist_ok=True)
    
    src_files = [
        os.path.join(root_dir, r"kernel\main.c"),
        os.path.join(root_dir, r"kernel\process.c"),
        os.path.join(root_dir, r"arch\x86_64\syscall_dispatcher.c"),
        os.path.join(root_dir, r"subsystems\win_subsystem.c"),
        os.path.join(root_dir, r"subsystems\linux_subsystem.c"),
        os.path.join(root_dir, r"loaders\pe_loader.c"),
        os.path.join(root_dir, r"loaders\elf_loader.c"),
        os.path.join(root_dir, r"drivers\vdm_bridge.c")
    ]
    
    out_exe = os.path.join(temp_build_dir, "hybridos_core.exe")
    include_arg1 = f"/I{os.path.join(root_dir, 'include')}"
    include_arg2 = f"/I{root_dir}"
    
    cl_args = [
        cl_path,
        "/nologo",
        "/W3",
        "/O2",
        "/D_CRT_SECURE_NO_WARNINGS",
        include_arg1,
        include_arg2,
        f"/Fe:{out_exe}",
        f"/Fo:{temp_build_dir}\\"
    ] + src_files
    
    print("Compiling HybridOS Kernel modules with cl.exe...")
    build_proc = subprocess.run(cl_args, env=msvc_env, capture_output=True, text=True, cwd=temp_build_dir)
    
    print("BUILD OUTPUT:")
    print(build_proc.stdout)
    if build_proc.stderr:
        print("BUILD STDERR:")
        print(build_proc.stderr)
        
    if build_proc.returncode != 0:
        print(f"Build failed with exit code: {build_proc.returncode}")
        return build_proc.returncode
        
    print("\n========================================================")
    print(">>> RUNNING HYBRIDOS KERNEL (hybridos_core.exe)...")
    print("========================================================\n")
    
    run_proc = subprocess.run([out_exe], capture_output=True, text=True, cwd=temp_build_dir)
    print(run_proc.stdout)
    if run_proc.stderr:
        print("RUNTIME STDERR:")
        print(run_proc.stderr)

    # Copy output to root_dir\bin if possible
    try:
        bin_dir = os.path.join(root_dir, "bin")
        shutil.copy2(out_exe, os.path.join(bin_dir, "hybridos_core.exe"))
        print(f"Binary installed to: {os.path.join(bin_dir, 'hybridos_core.exe')}")
    except Exception as e:
        print(f"Notice: Executable is ready at {out_exe} ({e})")

    return run_proc.returncode

if __name__ == "__main__":
    sys.exit(main() or 0)
