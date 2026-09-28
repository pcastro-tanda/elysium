class Test
  public get_method_name_1 if get_method_name_1 =~ /a/
  public def bar; end
  ^^^^^^ `public` should not be inlined in method definitions.
  public get_method_name_2 if get_method_name_2 =~ /b/
end
