# Mob's thoughts

- load requires two readers
  - actually creates DB that has refs into what is reading from
  - readers should outlive Self returned by load
  - in the mainline code this is valid because binary data in memory is created and leaked
- Db::store_movie requires Title with static lifetime
  - might be too strong
- if we want to create Dbs, we need a storage that outlives ServiceDB itself
  - storage provided by the caller

# Some terminology

Backing store:: Thing that is dealing with reading content and writing content to/from disk and to/from vector.
ServiceDB (Current implementation):: Thing that provides the query API and contacts the backing store.
ServiceDB (Future Interface goal):: Generic trait supporting different backing stores.
Database:: 

Separate import of TSV into a separate class.
Separate loading of binary file into a separate class.
ServiceDBFromBinary:: initialize movie/ series info from binary and provide a query API on it.
