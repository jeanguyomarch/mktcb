/* This is part of mktcb - which is under the MIT License ********************/

use std::io::Write;

use crate::error::Result;
use crate::error;

use indicatif::{ProgressBar, ProgressStyle};
use snafu::{ResultExt, ensure};
use log::*;
use curl::easy::Easy;

pub fn check(handle: &mut Easy, url: &url::Url) -> Result<bool> {
    debug!("Checking if patch is available at {:#?}", url);
    handle.url(url.as_str())
        .context(error::URLError{url: url.clone()})?;
    handle.perform()
        .context(error::RequestError{url: url.clone()})?;
    let code = handle.response_code()
        .context(error::RequestError{url: url.clone()})?;


    // We have joined the server and performed a request. If we get
    // a hit (200), the file is available. If we get 404, we know for
    // sure the file is not there, move along.
    // For the other cases, it may be trickier: is the file actually
    // there, but did we run into a network error? To simplify, and
    // because I lack expertise here (what about reditections?), we
    // will consider it as a success with no update.
    match code {
        200 => Ok(true),
        404 => Ok(false),
        _ => Ok(false),
    }
}

pub fn to_file(handle: &mut Easy, url: &url::Url, path: &std::path::PathBuf) -> Result<()> {
    handle.url(url.as_str()).context(error::URLError{url: url.clone()})?;

    let mut file = std::fs::File::create(&path).context(
        error::CreateFileError{ path: path.clone() }
    )?;

    let pb = ProgressBar::new(0);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
        .progress_chars("#>-"));

    handle.progress(true).context(error::CURLSetupError{})?;
    {
        let mut transfer = handle.transfer();
        transfer.progress_function(|total, dl, _, _| {
            pb.set_length(total as u64);
            pb.set_position(dl as u64);
            true
        }).context(error::CURLSetupError{})?;
        transfer.write_function(|data| {
            // TODO - I have no idea how to handle the error here. The closure
            // expects us to return a WriteError,
            // https://docs.rs/curl/0.5.0/curl/easy/enum.WriteError.html
            // but it only has one field: pause, which is definitely not what I
            // expect to return.
            // So we just hope for the best...
            file.write_all(data).unwrap();
            Ok(data.len())
        }).context(error::CURLSetupError{})?;

        // And start the download!!!    info!("Downloading file from {}", url);
        transfer.perform().context(error::RequestError{url: url.clone()})?;
    }

    // Now that we have performed the transfer (or failed it!!) query the
    // return code to raise a proper error.
    let code = handle.response_code()
        .context(error::RequestError{url: url.clone()})?;
    ensure!(code == 200, error::DownloadError{
        url: url.clone(),
        code: code,
    });
    Ok(())
}
