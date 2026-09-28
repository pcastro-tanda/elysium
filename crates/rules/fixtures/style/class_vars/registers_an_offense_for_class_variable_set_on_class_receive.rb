class TestClass; end
TestClass.class_variable_set(:@@test, 42)
                             ^^^^^^^ Replace class var :@@test with a class instance var.
