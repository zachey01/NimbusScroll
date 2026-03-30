import tkinter as tk
from tkinter import ttk, messagebox
import os
import multiprocessing
import threading
from win32con import VK_SHIFT
import sv_ttk
import easing_functions
from SmoothedScroll import SmoothedScroll, SmoothedScrollConfig, AppConfig, ScrollConfig
import pystray
from pystray import MenuItem as item
from PIL import Image
import win32gui
import win32process
import psutil
import sys
import webbrowser

from utils.blocklist import load_blocklist, write_blocklist, toggle_blocklist
from utils.config import load_config, save_config, DEFAULT_CONFIG 
from utils.find_games import find_games

BLOCKLIST_PATH = os.path.join(os.getenv('APPDATA'), 'SmoothedScroll', 'blocklist.json')
APP_DATA_PATH = os.path.join(os.getenv('APPDATA'), 'SmoothedScroll')
ICON_PATH = os.path.join(os.path.dirname(__file__), 'assets', 'icon.ico')

def smoothed_scroll_task(config: SmoothedScrollConfig):
    try:
        smoothed_scroll_instance = SmoothedScroll(config=config)
        smoothed_scroll_instance.start(is_block=True)
    except Exception as e:
        print(f"Error in SmoothedScroll process: {e}")

class ScrollConfigApp:
    instance = None

    def __new__(cls, *args, **kwargs):
        if cls.instance is None:
            cls.instance = super(ScrollConfigApp, cls).__new__(cls)
        return cls.instance

    def __init__(self):
        if hasattr(self, 'initialized') and self.initialized:
            return
        self.initialized = True
        self.config = load_config()  
        self.root = tk.Tk()
        self.root.title("Smoothed Scroll Settings")
        self.root.iconbitmap(ICON_PATH)
        self.root.geometry("400x890")
        self.root.resizable(False, False)
        self.center_window()
        sv_ttk.set_theme(self.config.get("theme", "dark"))
        self.create_variables()
        self.smoothed_scroll_process = None
        self.setup_gui()
        self.root.protocol("WM_DELETE_WINDOW", self.on_closing)
        self.root.withdraw()
        self.smooth_scroll_started = False

    def create_variables(self):
        self.distance_var = tk.IntVar(value=self.config.get("scroll_distance", 120))
        self.acceleration_var = tk.DoubleVar(value=self.config.get("acceleration", 1.0))
        self.opposite_acceleration_var = tk.DoubleVar(value=self.config.get("opposite_acceleration", 1.2))
        self.acceleration_delta_var = tk.IntVar(value=self.config.get("acceleration_delta", 70))
        self.acceleration_max_var = tk.IntVar(value=self.config.get("acceleration_max", 14))
        self.duration_var = tk.IntVar(value=self.config.get("scroll_duration", 500))
        self.pulse_scale_var = tk.DoubleVar(value=self.config.get("pulse_scale", 3.0))
        self.inverted_scroll_var = tk.BooleanVar(value=self.config.get("inverted_scroll", False))
        self.theme_var = tk.StringVar(value=self.config.get("theme", "dark"))
        self.autostart_var = tk.BooleanVar(value=self.config.get("autostart", False))

    def center_window(self):
        self.root.update_idletasks()
        width = self.root.winfo_width()
        height = self.root.winfo_height()
        x = (self.root.winfo_screenwidth() // 2) - (width // 2)
        y = (self.root.winfo_screenheight() // 2) - (height // 2)
        self.root.geometry(f"{width}x{height}+{x}+{y}")

    def setup_gui(self):
        frame = ttk.LabelFrame(self.root, text="Scroll Settings")
        frame.pack(padx=10, pady=10, fill="x")
        self.create_scroll_settings(frame)

        theme_frame = ttk.LabelFrame(self.root, text="Theme Settings")
        theme_frame.pack(padx=10, pady=10, fill="x")
        self.create_theme_settings(theme_frame)

        other_frame = ttk.LabelFrame(self.root, text="Other")
        other_frame.pack(padx=10, pady=10, fill="x")
        self.create_donation_link(other_frame)
        self.create_autostart_option(frame)

    def create_scroll_settings(self, frame):
        ttk.Label(frame, text="Scroll Distance (px):").pack(anchor="w", padx=5, pady=5)
        ttk.Spinbox(frame, from_=0, to=2000, textvariable=self.distance_var).pack(anchor="w", fill="x", padx=5, pady=5)
        ttk.Label(frame, text="Acceleration (x):").pack(anchor="w", padx=5, pady=5)
        ttk.Entry(frame, textvariable=self.acceleration_var).pack(anchor="w", fill="x", padx=5, pady=5)
        ttk.Label(frame, text="Opposite Acceleration (x):").pack(anchor="w", padx=5, pady=5)
        ttk.Entry(frame, textvariable=self.opposite_acceleration_var).pack(anchor="w", fill="x", padx=5, pady=5)
        ttk.Label(frame, text="Acceleration Delta (ms):").pack(anchor="w", padx=5, pady=5)
        ttk.Entry(frame, textvariable=self.acceleration_delta_var).pack(anchor="w", fill="x", padx=5, pady=5)
        ttk.Label(frame, text="Max Acceleration Steps:").pack(anchor="w", padx=5, pady=5)
        ttk.Spinbox(frame, from_=0, to=30, textvariable=self.acceleration_max_var).pack(anchor="w", fill="x", padx=5, pady=5)
        ttk.Label(frame, text="Scroll Duration (ms):").pack(anchor="w", padx=5, pady=5)
        ttk.Spinbox(frame, from_=0, to=1000, textvariable=self.duration_var).pack(anchor="w", fill="x", padx=5, pady=5)
        ttk.Label(frame, text="Pulse Scale (x):").pack(anchor="w", padx=5, pady=5)
        ttk.Entry(frame, textvariable=self.pulse_scale_var).pack(anchor="w", fill="x", padx=5, pady=5)
        ttk.Checkbutton(frame, text="Inverted Scroll", variable=self.inverted_scroll_var).pack(anchor="w", padx=5, pady=5)
        self.action_button = ttk.Button(frame, text="Start Smoothed Scroll", command=self.toggle_smoothed_scroll)
        self.action_button.pack(fill="x", pady=5)
        ttk.Button(frame, text="Reset to Default", command=self.reset_to_default).pack(fill="x", pady=5)

    def create_theme_settings(self, frame):
        ttk.Radiobutton(frame, text="Dark Theme", variable=self.theme_var, value="dark", command=self.apply_theme).pack(anchor="w", padx=5, pady=5)
        ttk.Radiobutton(frame, text="Light Theme", variable=self.theme_var, value="light", command=self.apply_theme).pack(anchor="w", padx=5, pady=5)

    def create_autostart_option(self, frame):
        ttk.Checkbutton(frame, text="Enable Autostart", variable=self.autostart_var, command=self.toggle_autostart).pack(anchor="w", padx=5, pady=5)

    def create_donation_link(self, frame):
        ttk.Button(frame, text="Support me", command=self.open_donation_link).pack(anchor="w", padx=5, pady=5)

    def open_donation_link(self):
        webbrowser.open("https://www.donationalerts.com/r/zachey")

    def toggle_smoothed_scroll(self):
        if self.smoothed_scroll_process and self.smoothed_scroll_process.is_alive():
            self.stop_smoothed_scroll()
        else:
            self.apply_settings()

    def apply_settings(self):
        if self.smoothed_scroll_process and self.smoothed_scroll_process.is_alive():
            self.stop_smoothed_scroll()
        self.config["scroll_distance"] = self.distance_var.get()
        self.config["acceleration"] = self.acceleration_var.get()
        self.config["opposite_acceleration"] = self.opposite_acceleration_var.get()
        self.config["acceleration_delta"] = self.acceleration_delta_var.get()
        self.config["acceleration_max"] = self.acceleration_max_var.get()
        self.config["scroll_duration"] = self.duration_var.get()
        self.config["pulse_scale"] = self.pulse_scale_var.get()
        self.config["inverted_scroll"] = self.inverted_scroll_var.get()
        self.config["autostart"] = self.autostart_var.get()
        save_config(self.config) 
        self.start_smoothed_scroll()

    def toggle_autostart(self):
        self.config["autostart"] = self.autostart_var.get()
        save_config(self.config) 
        self.manage_autostart()

    def manage_autostart(self):
        startup_folder = os.path.join(os.getenv('APPDATA'), 'Microsoft', 'Windows', 'Start Menu', 'Programs', 'Startup')
        exe_path = os.path.abspath(sys.argv[0])
        shortcut_path = os.path.join(startup_folder, "SmoothedScroll.lnk")

        if self.config["autostart"]:
            self.create_shortcut(exe_path, shortcut_path)
        else:
            try:
                os.remove(shortcut_path)
            except FileNotFoundError:
                pass

    def create_shortcut(self, exe_path, shortcut_path):
        import winshell  
        with winshell.shortcut(shortcut_path) as shortcut:
            shortcut.path = exe_path
            shortcut.working_directory = os.path.dirname(exe_path)
            shortcut.description = "Smoothed Scroll Autostart"
            shortcut.icon_location = (exe_path, 0)

    def apply_theme(self):
        theme = self.theme_var.get()
        sv_ttk.set_theme(theme)
        self.config["theme"] = theme
        save_config(self.config) 

    def start_smoothed_scroll(self):
        if self.smoothed_scroll_process and self.smoothed_scroll_process.is_alive():
            return
        app_configs = [
            AppConfig(
                regexp=r'.*',
                scroll_config=ScrollConfig(
                    distance=self.distance_var.get(),
                    acceleration=self.acceleration_var.get(),
                    opposite_acceleration=self.opposite_acceleration_var.get(),
                    acceleration_delta=self.acceleration_delta_var.get(),
                    acceleration_max=self.acceleration_max_var.get(),
                    duration=self.duration_var.get(),
                    pulse_scale=self.pulse_scale_var.get(),
                    ease=easing_functions.LinearInOut,
                    inverted=self.inverted_scroll_var.get(),
                    horizontal_scroll_key=VK_SHIFT
                ),
            )
        ]
        smoothed_scroll_config = SmoothedScrollConfig(app_config=app_configs)
        self.smoothed_scroll_process = multiprocessing.Process(
            target=smoothed_scroll_task,
            args=(smoothed_scroll_config,),
            daemon=True
        )
        self.smoothed_scroll_process.start()
        self.smooth_scroll_started = True
        self.action_button.config(text="Stop Smoothed Scroll")

    def stop_smoothed_scroll(self):
        if self.smoothed_scroll_process and self.smoothed_scroll_process.is_alive():
            try:
                self.smoothed_scroll_process.terminate()
                self.smoothed_scroll_process.join(timeout=5)
            except Exception as e:
                print(f"Error terminating SmoothedScroll process: {e}")
            finally:
                self.smoothed_scroll_process = None
                self.smooth_scroll_started = False
                self.action_button.config(text="Start Smoothed Scroll")

    def reset_to_default(self):
        self.config = DEFAULT_CONFIG.copy()
        self.create_variables()
        self.apply_theme()

    def on_closing(self):
        if not self.config.get("message_shown", False):
            messagebox.showinfo(
                "Smoothed Scroll",
                "Smoothed Scroll is running from the system tray."
            )
            self.config["message_shown"] = True
            save_config(self.config)
        self.root.withdraw()

    def show(self):
        self.root.after(0, self._show)

    def _show(self):
        self.root.deiconify()
        self.root.lift()
        self.root.focus_force()

    def exit_app(self):
        self.stop_smoothed_scroll()
        self.root.quit()
        os._exit(0)

def load_icon():
    try:
        return Image.open(ICON_PATH)
    except Exception as e:
        print(f"Error loading icon: {e}")
        return None

def get_open_process_names():
    def enum_window_callback(hwnd, results):
        if win32gui.IsWindowVisible(hwnd) and win32gui.GetWindowText(hwnd):
            _, pid = win32process.GetWindowThreadProcessId(hwnd)
            try:
                process = psutil.Process(pid)
                process_name = process.name()
                results.add(process_name)
            except (psutil.NoSuchProcess, psutil.AccessDenied):
                pass
    processes = set()
    win32gui.EnumWindows(enum_window_callback, processes)
    return sorted(processes)

def build_menu(icon, app_instance):
    open_processes = get_open_process_names()
    blocklist = load_blocklist()
    process_items = [
        item(process, lambda _, p=process: toggle_blocklist(p),
             checked=lambda item, p=process: p in blocklist) for process in open_processes
    ]
    if not app_instance.smoothed_scroll_process or not app_instance.smoothed_scroll_process.is_alive():
        action_item = item("Start Smoothed Scroll", lambda _: app_instance.toggle_smoothed_scroll())
    else:
        action_item = item("Stop Smoothed Scroll", lambda _: app_instance.toggle_smoothed_scroll())
    icon.menu = pystray.Menu(
        action_item,
        item('Exceptions', pystray.Menu(*process_items)),
        item('Open Settings', lambda _: app_instance.show()),
        item('Exit', lambda _: (app_instance.exit_app(), icon.stop()))
    )
    icon.update_menu()

def run_tray(app_instance):
    icon_image = load_icon()
    if icon_image is None:
        return
    icon = pystray.Icon("SmoothedScroll", icon_image, "Smoothed Scroll")
    build_menu(icon, app_instance)
    icon.run()

def stop_icon(icon):
    icon.stop()

def run_system_tray(app_instance):
    tray_thread = threading.Thread(target=run_tray, args=(app_instance,), daemon=True)
    tray_thread.start()

def start_taskbar_icon():
    if not os.path.exists(APP_DATA_PATH):
        os.makedirs(APP_DATA_PATH, exist_ok=True)
    if not os.path.exists(BLOCKLIST_PATH):
        write_blocklist([])

if __name__ == "__main__":
    app = ScrollConfigApp()
    start_taskbar_icon()
    run_system_tray(app)
    app.root.mainloop()
    find_games()