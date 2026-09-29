map { |line| (line.match(/^ *#/) || line.strip.empty?) ? next : line }
