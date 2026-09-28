class Test
  private get_method_name_1 if get_method_name_1 =~ /a/
  private def bar; end
  ^^^^^^^ `private` should not be inlined in method definitions.
  private get_method_name_2 if get_method_name_2 =~ /b/
end
