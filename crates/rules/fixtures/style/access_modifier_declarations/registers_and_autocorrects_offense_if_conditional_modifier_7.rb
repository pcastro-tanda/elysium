class Test
  module_function get_method_name_1 if get_method_name_1 =~ /a/
  module_function def bar; end
  ^^^^^^^^^^^^^^^ `module_function` should not be inlined in method definitions.
  module_function get_method_name_2 if get_method_name_2 =~ /b/
end
