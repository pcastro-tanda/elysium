def test
  'foo'; rescue; 'baz'
         ^^^^^^ `rescue` at 2, 9 is not aligned with `def test` at 1, 0.
end

def test
  begin
    'foo'; rescue; 'baz'
           ^^^^^^ `rescue` at 7, 11 is not aligned with `begin` at 6, 2.
  end
end
