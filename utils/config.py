import json
import os

CONFIG_FILE_PATH = os.path.join(os.getenv('APPDATA'), 'SmoothedScroll', 'config.json')
DEFAULT_CONFIG = {
    "theme": "dark",
    "scroll_distance": 120,
    "acceleration": 1.0,
    "opposite_acceleration": 1.2,
    "acceleration_delta": 70,
    "acceleration_max": 14,
    "scroll_duration": 500,
    "pulse_scale": 3.0,
    "inverted_scroll": False,
    "autostart": False
}

def load_config():
    if not os.path.exists(CONFIG_FILE_PATH):
        return DEFAULT_CONFIG.copy()
    try:
        with open(CONFIG_FILE_PATH, 'r') as file:
            config = json.load(file)
            return config
    except (json.JSONDecodeError, FileNotFoundError):
        return DEFAULT_CONFIG.copy()

def save_config(config):
    with open(CONFIG_FILE_PATH, 'w') as file:
        json.dump(config, file, indent=4)