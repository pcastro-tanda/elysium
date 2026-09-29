if Integer === x || ((x == 2) || (3 == x)) || x =~ /foo/
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Convert `if-elsif` to `case-when`.
elsif 1 == x || (1..10) === x || x.match?(/bar/)
else
end
