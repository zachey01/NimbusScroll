import psutil
import time
import os
from utils.blocklist import write_blocklist, load_blocklist

def find_games():
    while True: 
        try:
            for proc in psutil.process_iter(['pid', 'name', 'exe']):
                if "steam" in proc.info['name'].lower() and "steamwebhelper" not in proc.info['name'].lower() and proc.info['exe']:
                    parent = psutil.Process(proc.info['pid'])
                    children = parent.children(recursive=True)
                    for child in children:
                        if child.exe() and "steamwebhelper" not in child.name().lower():
                            exe_name = os.path.splitext(os.path.basename(child.exe()))[0] + '.exe'
                            blocklist = load_blocklist()
                            if exe_name not in blocklist:
                                blocklist.append(exe_name)  
                                write_blocklist(blocklist) 
                            else:
                                print(f"{exe_name}")

        except (psutil.NoSuchProcess, psutil.AccessDenied, psutil.ZombieProcess):
            continue
        time.sleep(5)