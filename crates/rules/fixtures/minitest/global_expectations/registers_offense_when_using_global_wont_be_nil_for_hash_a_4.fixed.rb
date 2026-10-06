it 'does something' do
  n = do_something
  value(n[:foo]).wont_be_nil 42
end
