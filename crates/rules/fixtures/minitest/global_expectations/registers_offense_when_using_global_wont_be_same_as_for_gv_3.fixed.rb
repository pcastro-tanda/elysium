it 'does something' do
  $n = do_something
  _($n).wont_be_same_as 42
end
