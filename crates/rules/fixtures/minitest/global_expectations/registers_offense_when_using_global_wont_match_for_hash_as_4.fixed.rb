it 'does something' do
  n = do_something
  value(n[:foo]).wont_match 42
end
