class Test
  public get_method_name if get_method_name =~ /a/
  public def bar; end
  ^^^^^^ `public` should not be inlined in method definitions.
end
