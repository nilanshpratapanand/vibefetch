import yt_dlp
import sys
import json
import os

def get_ffmpeg_path():
    base_dir = os.path.dirname(os.path.abspath(__file__))
    ffmpeg_exe = os.path.join(base_dir, "ffmpeg.exe")
    if os.path.exists(ffmpeg_exe):
        return ffmpeg_exe
    return None

def progress_hook(d):
    if d['status'] == 'downloading':
        p = d.get('_percent_str', '0%').replace('%','').strip()
        data = {
            "status": "downloading",
            "percentage": p,
            "speed": d.get('_speed_str', '0B/s'),
            "eta": d.get('_eta_str', '00:00')
        }
        print(f"PROGRESS:{json.dumps(data)}", flush=True)

def download_video(url):
    download_path = os.path.join(os.path.expanduser("~"), "Downloads", "VibeFetch")
    if not os.path.exists(download_path): os.makedirs(download_path)

    ffmpeg_path = get_ffmpeg_path()
    
    # COMPATIBILITY LOGIC: 
    # 'bestvideo[vcodec^=avc1]' ensures we get H.264 (Compatible with everything)
    # 'bestaudio[ext=m4a]' ensures we get standard AAC audio
    ydl_opts = {
        'format': 'bestvideo[vcodec^=avc1]+bestaudio[ext=m4a]/best[ext=mp4]/best',
        'merge_output_format': 'mp4',
        'outtmpl': f'{download_path}/%(title)s.%(ext)s',
        'progress_hooks': [progress_hook],
        'quiet': True,
        'no_warnings': True,
    }

    if ffmpeg_path:
        ydl_opts['ffmpeg_location'] = ffmpeg_path
        print(f"DEBUG: Using FFmpeg at {ffmpeg_path}", flush=True)

    try:
        with yt_dlp.YoutubeDL(ydl_opts) as ydl:
            info = ydl.extract_info(url, download=True)
            print(json.dumps({"status": "success", "title": info.get('title')}))
    except Exception as e:
        print(json.dumps({"status": "error", "message": str(e)}))

if __name__ == "__main__":
    if len(sys.argv) > 1:
        download_video(sys.argv[1])