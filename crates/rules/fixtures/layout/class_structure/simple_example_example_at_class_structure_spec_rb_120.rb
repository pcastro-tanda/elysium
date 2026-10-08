class Person
  CONST = 'wrong place'
  include AnotherModule
  ^^^^^^^^^^^^^^^^^^^^^ `module_inclusion` is supposed to appear before `constants`.
  extend SomeModule
end
