begin
  foo
rescue *[First, Second]
       ^^^^^^^^^^^^^^^^ Replace splat expansion with comma separated values.
  bar
end
