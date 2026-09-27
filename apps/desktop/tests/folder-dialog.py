"""Interact with the actual Windows folder dialog on the test process desktop.

This helper never switches desktops or sends global keyboard/mouse input.
"""

import sys
import time

from pywinauto import Desktop


def find_dialog(title):
    desktop = Desktop(backend="win32")
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        for window in desktop.windows():
            if window.window_text() == title:
                return window
        time.sleep(0.2)
    print("WINDOWS/Win32:", [(w.window_text(), w.element_info.class_name) for w in desktop.windows()], flush=True)
    raise RuntimeError("Windows dialog did not appear on the isolated desktop")


mode = sys.argv[1]
dialog = find_dialog("Escolha um arquivo deste projeto" if mode == "file-select" else "Escolha a pasta do projeto")
if sys.argv[1] == "cancel":
    dialog.close()
elif sys.argv[1] == "select":
    dialog.descendants(class_name="Edit")[0].set_edit_text(sys.argv[2])
    dialog.descendants(title="Selecionar pasta", class_name="Button")[0].click()
elif mode == "file-select":
    try:
        dialog.descendants(class_name="Edit")[0].set_edit_text(sys.argv[2])
        [button for button in dialog.descendants(class_name="Button") if button.window_text().replace("&", "") == "Abrir"][0].click()
    except (IndexError, RuntimeError):
        print("CONTROLS:", [(c.window_text(), c.element_info.class_name) for c in dialog.descendants()], flush=True)
        raise
else:
    raise RuntimeError("Unsupported dialog test mode")
