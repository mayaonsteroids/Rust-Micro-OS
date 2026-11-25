import os
import zipfile

def package_repo(source_dir, output_zip_path, exclude_names=None):
    if exclude_names is None:
        exclude_names = set()
    
    with zipfile.ZipFile(output_zip_path, 'w', zipfile.ZIP_DEFLATED) as zipf:
        for root, dirs, files in os.walk(source_dir):
            dirs[:] = [d for d in dirs if d not in ('.git', '__pycache__', '.pytest_cache')]
            for file in sorted(files):
                if file in exclude_names or file.endswith(('.pyc', '.pyo', '.tmp')):
                    continue
                full_path = os.path.join(root, file)
                rel_path = os.path.relpath(full_path, source_dir)
                zipf.write(full_path, rel_path)

if __name__ == '__main__':
    sub_dir = '/mnt/e/MICRO OS/submission'
    staging = '/tmp/staging'

    # 1. Performance-Investigation.zip
    pi_src = os.path.join(staging, 'Performance-Investigation')
    pi_out = os.path.join(sub_dir, 'Performance-Investigation.zip')
    package_repo(pi_src, pi_out, {'PUT_REPORT_HERE.txt', 'PUT_SCREENSHOTS_HERE.txt', '.gitignore'})
    print('Created:', pi_out)

    # 2. CA5-Linux-Process-Investigator.zip
    lpi_src = os.path.join(staging, 'CA5-Linux-Process-Investigator')
    lpi_out = os.path.join(sub_dir, 'CA5-Linux-Process-Investigator.zip')
    package_repo(lpi_src, lpi_out, {'.gitignore'})
    print('Created:', lpi_out)

    # 3. CA3-Process-vs-Thread.zip
    pvt_src = os.path.join(staging, 'CA3-Process-vs-Thread')
    pvt_out = os.path.join(sub_dir, 'CA3-Process-vs-Thread.zip')
    package_repo(pvt_src, pvt_out, {'.gitignore'})
    print('Created:', pvt_out)
