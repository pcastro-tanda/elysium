while /foo/
      ^^^^^ Do not use regexp literal as a condition. The regexp literal matches `$_` implicitly.
  bar
end
