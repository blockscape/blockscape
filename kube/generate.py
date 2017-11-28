import fileinput
import sys

for i in range(int(sys.argv[1])):
    with fileinput.FileInput('blockscape-client.yaml') as file:

        with open('blockscape-client-' + str(i) + '.yaml', 'w') as fo:
            for line in file:
                fo.write(line.replace('%%', str(i)))
            
