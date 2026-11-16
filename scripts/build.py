"""Run with the project environment on the target OS: python scripts/build.py."""
from pathlib import Path
import platform
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def main():
    if sys.platform not in ('win32', 'linux', 'darwin'):
        raise SystemExit('Run this build on Windows, Linux or macOS. Cross-compilation is not supported.')
    from PIL import Image
    output = ROOT / 'dist'
    output.mkdir(exist_ok=True)
    icon = ROOT / 'virtual_mixer/assets/icon.png'
    ico = output / 'icon.ico'
    with Image.open(icon) as image:
        image.save(ico, sizes=[(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    command = [sys.executable, '-m', 'PyInstaller', '--noconfirm', '--clean',
               '--onefile', '--windowed', '--name', 'VirtualMixer',
               '--paths', str(ROOT), '--collect-data', 'virtual_mixer',
               '--collect-all', '_sounddevice_data', '--collect-all', '_soundfile_data']
    if sys.platform == 'win32':
        command += ['--icon', str(ico)]
    elif sys.platform == 'darwin':
        icns = output / 'icon.icns'
        with Image.open(icon) as image:
            image.save(icns)
        command += ['--icon', str(icns), '--osx-bundle-identifier', 'com.dontabusesounds.virtualmixer']
    else:
        candidates = [Path('/usr/lib/libportaudio.so.2')]
        for directory in ('/usr/lib', '/lib'):
            candidates.extend(Path(directory).glob('*/libportaudio.so.2'))
        library = next((p.resolve() for p in candidates if p.exists()), None)
        if library is None:
            raise SystemExit('Install libportaudio2 before building (sudo apt-get install libportaudio2).')
        command += ['--add-binary', f'{library}:.']
    command.append(str(ROOT / 'scripts/launcher.py'))
    subprocess.run(command, cwd=ROOT, check=True)
    suffix = '.exe' if sys.platform == 'win32' else ''
    package = output / f'VirtualMixer-{sys.platform}-{platform.machine()}'
    package.mkdir(exist_ok=True)
    if sys.platform == 'darwin':
        # ditto preserves the app bundle's symlinks and executable permissions.
        archive = output / f'{package.name}.zip'
        subprocess.run(['ditto', '-c', '-k', '--sequesterRsrc', '--keepParent',
                        str(output / 'VirtualMixer.app'), str(archive)], check=True)
        print(f'Build created: {archive}')
        return
    shutil.copy2(output / f'VirtualMixer{suffix}', package)
    shutil.copy2(icon, package / 'VirtualMixer.png')
    shutil.copy2(ROOT / 'README.md', package)
    if sys.platform == 'linux':
        shutil.copy2(ROOT / 'scripts/VirtualMixer.desktop', package)
        shutil.copy2(ROOT / 'scripts/install-desktop.sh', package)
        (package / 'install-desktop.sh').chmod(0o755)
        shutil.make_archive(str(package), 'gztar', root_dir=output, base_dir=package.name)
    else:
        shutil.make_archive(str(package), 'zip', root_dir=output, base_dir=package.name)
    print(f'Build created in {package}')


if __name__ == '__main__':
    main()
