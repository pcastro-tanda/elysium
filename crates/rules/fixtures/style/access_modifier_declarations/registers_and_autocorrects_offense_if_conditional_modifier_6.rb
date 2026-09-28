class Test
  module_function get_method_name if get_method_name =~ /a/
  module_function def bar; end
  ^^^^^^^^^^^^^^^ `module_function` should not be inlined in method definitions.
end
