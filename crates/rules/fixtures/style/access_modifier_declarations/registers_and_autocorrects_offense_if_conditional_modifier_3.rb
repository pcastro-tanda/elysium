class Test
  protected get_method_name_1 if get_method_name_1 =~ /a/
  protected def bar; end
  ^^^^^^^^^ `protected` should not be inlined in method definitions.
  protected get_method_name_2 if get_method_name_2 =~ /b/
end
