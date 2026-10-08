def foo
  return $1 unless foo.match(/re/, 1)
end
