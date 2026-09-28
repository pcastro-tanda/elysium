class Test
  private attr_writer :foo
  ^^^^^^^ `private` should not be inlined in method definitions.
end
