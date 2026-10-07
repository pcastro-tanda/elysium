it 'does something' do
  n = do_something
  value(n[:foo]).wont_be_same_as 42
end
