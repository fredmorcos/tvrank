use indoc::indoc;
use std::io::BufRead;

pub(crate) fn make_basics_reader() -> impl BufRead {
  indoc! {"
      tconst\ttitleType\tprimaryTitle\toriginalTitle\tisAdult\tstartYear\tendYear\truntimeMinutes\tgenres
      tt0000001\tshort\tCarmencita\tCarmencita\t0\t1894\t\\N\t1\tDocumentary,Short
      tt0000002\tshort\tLe clown et ses chiens\tLe clown et ses chiens\t0\t1892\t\\N\t5\tAnimation,Short
      tt0000003\tshort\tPauvre Pierrot\tPauvre Pierrot\t0\t1892\t\\N\t4\tAnimation,Comedy,Romance
      tt0000004\tshort\tUn bon bock\tUn bon bock\t0\t1892\t\\N\t12\tAnimation,Short
      tt0000005\tshort\tBlacksmith Scene\tBlacksmith Scene\t0\t1893\t\\N\t1\tComedy,Short
      tt0000006\tshort\tChinese Opium Den\tChinese Opium Den\t0\t1894\t\\N\t1\tShort
      tt0000007\tshort\tCorbett and Courtney Before the Kinetograph\tCorbett and Courtney Before the Kinetograph\t0\t1894\t\\N\t1\tShort,Sport
      tt0000008\tshort\tEdison Kinetoscopic Record of a Sneeze\tEdison Kinetoscopic Record of a Sneeze\t0\t1894\t\\N\t1\tDocumentary,Short
      tt0000009\tshort\tMiss Jerry\tMiss Jerry\t0\t1894\t\\N\t40\tRomance,Short
      tt0000010\tshort\tLeaving the Factory\tLa sortie de l'usine Lumière à Lyon\t0\t1895\t\\N\t1\tDocumentary,Short
      tt0212278\tshort\tKineto's Side-Splitters No. 1\tKineto's Side-Splitters No. 1\t0\t1915\t\\N\t\\N\tShort
    "}.as_bytes()
}

pub(crate) fn make_ratings_reader() -> impl BufRead {
  indoc! {"
      tconst\taverageRating\tnumVotes
      tt0000001\t5.7\t1845
      tt0000002\t6.0\t236
      tt0000003\t6.5\t1603
      tt0000004\t6.0\t153
      tt0000005\t6.2\t2424
      tt0000006\t5.2\t158
      tt0000007\t5.4\t758
      tt0000008\t5.5\t1988
      tt0000009\t5.9\t191
      tt0000010\t6.9\t6636
    "}
  .as_bytes()
}
