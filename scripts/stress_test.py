import urllib.request
import threading
import time

url = "http://127.0.0.1:7878/echo"

def send_request(request_id):
    start = time.time()
    try:
        req = urllib.request.Request(
            url,
            data=b'{"message": "stress test"}',
            headers={"Content-Type": "application/json"},
            method="POST"
        )
        with urllib.request.urlopen(req) as response:
            response.read()
            print(f"Request {request_id} finished in {time.time() - start:.2f}s")
    except Exception as e:
        print(f"Request {request_id} failed: {e}")

threads = []
print("Spawning 5 concurrent POST requests to /echo...")
start_time = time.time()

for i in range(5):
    t = threading.Thread(target=send_request, args=(i,))
    threads.append(t)
    t.start()

for t in threads:
    t.join()

print(f"Total time taken: {time.time() - start_time:.2f}s")
