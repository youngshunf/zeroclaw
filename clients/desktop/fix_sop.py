import re

with open('src/huanxing/pages/SopWorkbench.tsx', 'r') as f:
    content = f.read()

# I will just write a python script that does the manual replacements carefully since regex for JS objects is tricky.
# Wait, let's just use Python to replace the known blocks or rewrite the whole return statement.
