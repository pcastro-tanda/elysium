class Test
  private get_method_name if get_method_name =~ /a/
  private def bar; end
  ^^^^^^^ `private` should not be inlined in method definitions.
end
