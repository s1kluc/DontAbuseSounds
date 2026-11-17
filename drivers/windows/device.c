#define UNICODE
#define _UNICODE
#include <windows.h>
#include <setupapi.h>
#include <newdev.h>
#include <stdio.h>
#include <wchar.h>

static const WCHAR hardware_id[] = L"ROOT\\DontAbuseSoundsMic\0";
static const GUID media = {0x4d36e96c, 0xe325, 0x11ce,
                            {0xbf, 0xc1, 0x08, 0x00, 0x2b, 0xe1, 0x03, 0x18}};

static DWORD find_device(HDEVINFO list, SP_DEVINFO_DATA *device, BOOL *found) {
    *found = FALSE;
    for (DWORD i = 0; SetupDiEnumDeviceInfo(list, i, device); ++i) {
        DWORD type, bytes;
        WCHAR ids[512] = {0};
        if (!SetupDiGetDeviceRegistryPropertyW(list, device, SPDRP_HARDWAREID, &type,
                                               (BYTE *)ids, sizeof(ids), &bytes) ||
            type != REG_MULTI_SZ || bytes > sizeof(ids) - sizeof(WCHAR)) continue;
        for (const WCHAR *p = ids; *p; p += wcslen(p) + 1) {
            if (_wcsicmp(p, hardware_id) == 0) {
                *found = TRUE;
                return ERROR_SUCCESS;
            }
        }
    }
    DWORD error = GetLastError();
    return error == ERROR_NO_MORE_ITEMS ? ERROR_SUCCESS : error;
}

static DWORD configure(const WCHAR *inf) {
    HDEVINFO list = SetupDiGetClassDevsW(&media, NULL, NULL, 0);
    if (list == INVALID_HANDLE_VALUE) return GetLastError();
    SP_DEVINFO_DATA device = {sizeof(device)};
    BOOL found, created = FALSE;
    DWORD result = find_device(list, &device, &found);
    if (result != ERROR_SUCCESS) goto done;
    if (!inf) {
        if (found && !SetupDiCallClassInstaller(DIF_REMOVE, list, &device))
            result = GetLastError();
        goto done;
    }
    if (!found) {
        if (!SetupDiCreateDeviceInfoW(list, L"DontAbuseSoundsMic", &media,
                                      L"DontAbuseSounds Virtual Audio", NULL,
                                      DICD_GENERATE_ID, &device) ||
            !SetupDiSetDeviceRegistryPropertyW(list, &device, SPDRP_HARDWAREID,
                                               (const BYTE *)hardware_id, sizeof(hardware_id)) ||
            !SetupDiCallClassInstaller(DIF_REGISTERDEVICE, list, &device)) {
            result = GetLastError();
            goto done;
        }
        created = TRUE;
    }
    BOOL restart = FALSE;
    if (!UpdateDriverForPlugAndPlayDevicesW(NULL, hardware_id, inf,
                                           INSTALLFLAG_FORCE | INSTALLFLAG_NONINTERACTIVE, &restart)) {
        result = GetLastError();
        if (created) SetupDiCallClassInstaller(DIF_REMOVE, list, &device);
    } else if (restart) {
        result = ERROR_SUCCESS_REBOOT_REQUIRED;
    }
done:
    SetupDiDestroyDeviceInfoList(list);
    return result;
}

int wmain(int argc, WCHAR **argv) {
    if (argc != 2) return ERROR_INVALID_PARAMETER;
    DWORD result = configure(wcscmp(argv[1], L"--remove") == 0 ? NULL : argv[1]);
    if (result && result != ERROR_SUCCESS_REBOOT_REQUIRED)
        fwprintf(stderr, L"DontAbuseSounds audio device setup failed (%lu)\n", result);
    return (int)result;
}
