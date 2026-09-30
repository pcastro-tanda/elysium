dm[i][j] = [
  dm[i-1][j-1] +
    (this[j-1] == that[i-1] ? 0 : sub),
  dm[i][j-1] + ins,
  dm[i-1][j] + del
].min
