foo
return 1, 2, 3 if 1 == 1
^^^^^^^^^^^^^^ Top level return with argument detected.
bar
return 2
^^^^^^^^ Top level return with argument detected.
return 3
^^^^^^^^ Top level return with argument detected.

def method
  return "Hello World" if 1 == 1
end
