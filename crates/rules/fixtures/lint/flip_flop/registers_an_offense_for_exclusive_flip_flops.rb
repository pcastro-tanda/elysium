DATA.each_line do |line|
print line if (line =~ /begin/)...(line =~ /end/)
              ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Avoid the use of flip-flop operators.
end
