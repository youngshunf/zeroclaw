import re
import sys

def convert_to_apply(css_content):
    prop_map = {
        'background': 'bg',
        'background-color': 'bg',
        'color': 'text',
        'border-color': 'border',
        'border-top-color': 'border-t',
        'border-bottom-color': 'border-b',
        'border-left-color': 'border-l',
        'border-right-color': 'border-r',
    }
    
    lines = css_content.split('\n')
    out_lines = []
    
    current_block = []
    in_block = False
    
    for line in lines:
        if '{' in line:
            in_block = True
            out_lines.append(line)
            continue
        if '}' in line:
            if current_block:
                applies = []
                other_lines = []
                for prop_line in current_block:
                    prop_line = prop_line.strip()
                    if not prop_line:
                        continue
                    m = re.match(r'([a-zA-Z-]+)\s*:\s*var\(--hx-([^)]+)\)\s*(?:!important\s*)?;', prop_line)
                    if m:
                        prop = m.group(1)
                        var = m.group(2)
                        is_important = '!important' in prop_line
                        prefix = '!' if is_important else ''
                        if prop in prop_map:
                            if prop == 'color' and var == 'text-primary':
                                applies.append(f"{prefix}text-hx-{var}")
                            elif prop == 'background' and var == 'purple-bg':
                                applies.append(f"{prefix}bg-hx-{var}")
                            else:
                                applies.append(f"{prefix}{prop_map[prop]}-hx-{var}")
                        elif prop == 'border-radius':
                            if var.startswith('radius-'):
                                applies.append(f"{prefix}rounded-hx-{var}")
                            elif var == 'radius':
                                applies.append(f"{prefix}rounded-hx-radius-md")
                            else:
                                other_lines.append('  ' + prop_line)
                        else:
                            other_lines.append('  ' + prop_line)
                    elif re.match(r'([a-zA-Z-]+)\s*:\s*(transparent|none)\s*(?:!important\s*)?;', prop_line):
                        prop = re.match(r'([a-zA-Z-]+)\s*:\s*(transparent|none)\s*(?:!important\s*)?;', prop_line).group(1)
                        val = re.match(r'([a-zA-Z-]+)\s*:\s*(transparent|none)\s*(?:!important\s*)?;', prop_line).group(2)
                        is_important = '!important' in prop_line
                        prefix = '!' if is_important else ''
                        if prop in prop_map and val == 'transparent':
                            applies.append(f"{prefix}{prop_map[prop]}-transparent")
                        elif prop == 'border' and val == 'none':
                            applies.append(f"{prefix}border-none")
                        elif prop == 'outline' and val == 'none':
                            applies.append(f"{prefix}outline-none")
                        else:
                            other_lines.append('  ' + prop_line)
                    else:
                        other_lines.append('  ' + prop_line)
                
                if applies:
                    out_lines.append("  @apply " + " ".join(applies) + ";")
                out_lines.extend(other_lines)
                current_block = []
            in_block = False
            out_lines.append(line)
            continue
            
        if in_block:
            current_block.append(line)
        else:
            out_lines.append(line)
            
    return '\n'.join(out_lines)

if __name__ == '__main__':
    with open('src/huanxing/styles/huanxing.css', 'r') as f:
        content = f.read()
    
    converted = convert_to_apply(content)
    
    with open('src/huanxing/styles/modules/components.css', 'w') as f:
        f.write('@config "../../../../tailwind.config.js";\n' + converted)
