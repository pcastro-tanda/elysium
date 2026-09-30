def foo
  puts 'some action happens here'
rescue => e
  a_check ? raise(e) : other_thing
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Add empty line after guard clause.
  true
end
