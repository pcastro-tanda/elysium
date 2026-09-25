def some_method
  foo = 1
  puts foo
  proc_taking_block = proc do |&foo|
                               ^^^^ Shadowing outer local variable - `foo`.
  end
  proc_taking_block.call do
  end
end
