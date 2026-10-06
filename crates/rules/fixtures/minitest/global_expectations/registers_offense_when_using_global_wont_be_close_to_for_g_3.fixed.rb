it 'does something' do
  $n = do_something
  _($n).wont_be_close_to 42
end
