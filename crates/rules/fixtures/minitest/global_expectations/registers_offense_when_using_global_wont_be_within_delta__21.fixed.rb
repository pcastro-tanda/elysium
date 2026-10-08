it 'does something' do
  @n = do_something
  _(@n[:foo]).wont_be_within_delta 42
end
