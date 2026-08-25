import time
from PIL import Image
import cv2

input("set single core thread affinity? ")
test = cv2.VideoCapture()
test.open("./test/source.mkv")
for i in range(10):  
    start = time.perf_counter()
    ret, frame = test.read()
    if not ret: raise Exception("L")
    print(f"frame {i} took {time.perf_counter() - start}")
    # Image.fromarray(frame[:,:,::-1], mode="RGB").save(f"./test/cap_py/{i}.jpg")
start = time.perf_counter()
test.set(cv2.CAP_PROP_POS_FRAMES, 86431)
print(f"seeking took {time.perf_counter() - start}")
for i in range(86431, 87531):
    start = time.perf_counter()
    ret, frame = test.read()
    if not ret: raise Exception("L")
    print(f"frame {i} took {time.perf_counter() - start}")
    # Image.fromarray(frame[:,:,::-1], mode="RGB").save(f"./test/cap_py/{i}.jpg")