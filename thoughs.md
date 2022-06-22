# Koushik's thoughts

- load requires two readers
  - actually creates DB that has refs into what is reading from
  - readers should outlive Self returned by load
  - in the mainline code this is valid because binary data in memory is created and leaked
- Db::store_movie requires Title with static lifetime
  - might be too strong
- if we want to create Dbs, we need a storage that outlives ServiceDB itself
  - storage provided by the caller
