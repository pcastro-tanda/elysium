it 'does something' do
  @n = do_something
  @n.must_include 42
  ^^ Use `_(@n)` instead.
end
