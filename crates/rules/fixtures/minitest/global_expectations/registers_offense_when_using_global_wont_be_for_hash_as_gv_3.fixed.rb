it 'does something' do
  $n = do_something
  _($n[:foo]).wont_be 42
end
