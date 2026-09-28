class Test
  module_function get_method_name_1 if get_method_name_1 =~ /a/
  module_function get_method_name_2 if get_method_name_2 =~ /b/
module_function

def bar; end
end
