map { |line| if line.match(/^ *#/) || line.strip.empty?
               next
             else
               line
             end }
