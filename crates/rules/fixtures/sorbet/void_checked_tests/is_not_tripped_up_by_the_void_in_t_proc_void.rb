sig { params(blk: T.proc.void).returns(T.anything).checked(:tests) }
def foo(&blk); end
