it 'does something' do
  @n = do_something
  _(@n[:foo]).wont_match 42
end
