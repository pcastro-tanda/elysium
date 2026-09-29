return nil unless (line =~ //) != nil
                  ^^^^^^^^^^^^^^^^^^^ Prefer `!(line =~ //).nil?` over `(line =~ //) != nil`.
