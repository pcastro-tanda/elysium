it 'does something' do
  $n = do_something
  _($n[:foo]).must_include 42
end
