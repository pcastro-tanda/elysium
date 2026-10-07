it 'does something' do
  $n = do_something
  _($n).wont_include 42
end
