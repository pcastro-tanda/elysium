class Test
  protected get_method_name if get_method_name =~ /a/
  protected def bar; end
  ^^^^^^^^^ `protected` should not be inlined in method definitions.
end
