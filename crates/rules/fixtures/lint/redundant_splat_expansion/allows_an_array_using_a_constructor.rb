begin
  foo
rescue *Array.new(3) { 42 }
  bad_example
end
