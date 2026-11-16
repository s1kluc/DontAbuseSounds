"""Run with the project environment on the target OS: python scripts/build.py."""
from pathlib import Path
import os
import platform
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def create_macos_dmg(app, destination):
    with tempfile.TemporaryDirectory() as staging:
        staging = Path(staging)
        subprocess.run(['ditto', str(app), str(staging / app.name)], check=True)
        (staging / 'Applications').symlink_to('/Applications')
        subprocess.run(['hdiutil', 'create', '-volname', 'Virtual Mixer',
                        '-srcfolder', str(staging), '-format', 'UDZO', '-fs', 'HFS+',
                        '-ov', str(destination)], check=True)
    subprocess.run(['hdiutil', 'verify', str(destination)], check=True)


def sign_windows(directory):
    thumbprint = os.environ.get('WINDOWS_CERTIFICATE_THUMBPRINT')
    if not thumbprint:
        print('WARNING: Windows package is unsigned; application control may block native modules.')
        return
    signtool = shutil.which('signtool')
    if not signtool:
        sdk = Path(os.environ.get('ProgramFiles(x86)', r'C:\Program Files (x86)')) / 'Windows Kits/10/bin'
        signtool = next(iter(sorted(sdk.glob('*/x64/signtool.exe'), reverse=True)), None)
    if not signtool:
        raise SystemExit('Install the Windows SDK (signtool.exe) to sign the Windows package.')
    for path in sorted(directory.rglob('*')):
        if not path.is_file() or path.suffix.lower() not in ('.exe', '.dll', '.pyd'):
            continue
        # Preserve trusted vendor signatures; sign every remaining native binary.
        verified = subprocess.run([str(signtool), 'verify', '/pa', str(path)],
                                  stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        if verified.returncode:
            subprocess.run([str(signtool), 'sign', '/sha1', thumbprint, '/fd', 'SHA256',
                            '/tr', 'http://timestamp.digicert.com', '/td', 'SHA256', str(path)], check=True)
        subprocess.run([str(signtool), 'verify', '/pa', str(path)], check=True)


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
               '--onedir' if sys.platform == 'win32' else '--onefile',
               '--windowed', '--name', 'VirtualMixer', '--noupx',
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
    if sys.platform == 'darwin':
        # ditto preserves the app bundle's symlinks and executable permissions.
        archive = output / f'{package.name}.zip'
        subprocess.run(['ditto', '-c', '-k', '--sequesterRsrc', '--keepParent',
                        str(output / 'VirtualMixer.app'), str(archive)], check=True)
        dmg = output / f'{package.name}.dmg'
        create_macos_dmg(output / 'VirtualMixer.app', dmg)
        print(f'Build created: {archive} and {dmg}')
        return
    if sys.platform == 'win32':
        windows_archive = package
        package = output / 'VirtualMixer'
        sign_windows(package)
    else:
        package.mkdir(exist_ok=True)
        shutil.copy2(output / f'VirtualMixer{suffix}', package)
    shutil.copy2(icon, package / 'VirtualMixer.png')
    shutil.copy2(ROOT / 'README.md', package)
    if sys.platform == 'linux':
        shutil.copy2(ROOT / 'scripts/VirtualMixer.desktop', package)
        shutil.copy2(ROOT / 'scripts/install-desktop.sh', package)
        (package / 'install-desktop.sh').chmod(0o755)
        shutil.make_archive(str(package), 'gztar', root_dir=output, base_dir=package.name)
    else:
        shutil.make_archive(str(windows_archive), 'zip', root_dir=output, base_dir=package.name)
    print(f'Build created in {package}')


if __name__ == '__main__':
    main()
