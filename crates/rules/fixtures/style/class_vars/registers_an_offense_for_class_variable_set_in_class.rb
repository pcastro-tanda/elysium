class TestClass
  class_variable_set(:@@test, 2)
                     ^^^^^^^ Replace class var :@@test with a class instance var.
end
